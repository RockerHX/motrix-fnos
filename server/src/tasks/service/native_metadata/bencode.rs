use std::collections::BTreeMap;

pub(super) struct Node<'a> {
    pub raw: &'a [u8],
    pub value: Value<'a>,
}

pub(super) enum Value<'a> {
    Bytes(&'a [u8]),
    Integer(i64),
    List(Vec<Node<'a>>),
    Dict(BTreeMap<&'a [u8], Node<'a>>),
}

impl<'a> Node<'a> {
    pub fn get(&self, key: &[u8]) -> Result<&Node<'a>, String> {
        self.dict()?.get(key).ok_or_else(invalid)
    }

    pub fn dict(&self) -> Result<&BTreeMap<&'a [u8], Node<'a>>, String> {
        match &self.value {
            Value::Dict(value) => Ok(value),
            _ => Err(invalid()),
        }
    }

    pub fn list(&self) -> Result<&[Node<'a>], String> {
        match &self.value {
            Value::List(value) => Ok(value),
            _ => Err(invalid()),
        }
    }

    pub fn bytes(&self) -> Result<&'a [u8], String> {
        match self.value {
            Value::Bytes(value) => Ok(value),
            _ => Err(invalid()),
        }
    }

    pub fn integer(&self) -> Result<u64, String> {
        match self.value {
            Value::Integer(value) => u64::try_from(value).map_err(|_| invalid()),
            _ => Err(invalid()),
        }
    }
}

pub(super) fn invalid() -> String {
    "task_metadata_missing：原生完整 metadata 无法安全核对，已保留任务和文件".to_string()
}

pub(super) fn parse(bytes: &[u8]) -> Result<Node<'_>, String> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(invalid());
    }
    let mut remaining = bytes;
    let mut budget = 100_000;
    let value = read(&mut remaining, 0, &mut budget)?;
    if !remaining.is_empty() {
        return Err(invalid());
    }
    Ok(value)
}

fn read<'a>(input: &mut &'a [u8], depth: usize, budget: &mut usize) -> Result<Node<'a>, String> {
    if depth > 64 || *budget == 0 {
        return Err(invalid());
    }
    *budget -= 1;
    let start = *input;
    let tag = *input.first().ok_or_else(invalid)?;
    let value = match tag {
        b'0'..=b'9' => Value::Bytes(read_bytes(input)?),
        b'i' => {
            let end = input.iter().position(|b| *b == b'e').ok_or_else(invalid)?;
            let number = std::str::from_utf8(&input[1..end]).map_err(|_| invalid())?;
            let value = number.parse::<i64>().map_err(|_| invalid())?;
            if value.to_string() != number {
                return Err(invalid());
            }
            *input = &input[end + 1..];
            Value::Integer(value)
        }
        b'l' | b'd' => {
            *input = &input[1..];
            let mut list = Vec::new();
            let mut dict = BTreeMap::new();
            while input.first() != Some(&b'e') {
                if tag == b'l' {
                    list.push(read(input, depth + 1, budget)?);
                } else {
                    let key = read_bytes(input)?;
                    if dict
                        .last_key_value()
                        .is_some_and(|(previous, _)| *previous >= key)
                    {
                        return Err(invalid());
                    }
                    dict.insert(key, read(input, depth + 1, budget)?);
                }
            }
            *input = &input[1..];
            if tag == b'l' {
                Value::List(list)
            } else {
                Value::Dict(dict)
            }
        }
        _ => return Err(invalid()),
    };
    Ok(Node {
        raw: &start[..start.len() - input.len()],
        value,
    })
}

fn read_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], String> {
    let colon = input.iter().position(|b| *b == b':').ok_or_else(invalid)?;
    let text = std::str::from_utf8(&input[..colon]).map_err(|_| invalid())?;
    let length = text.parse::<usize>().map_err(|_| invalid())?;
    if length.to_string() != text {
        return Err(invalid());
    }
    let end = (colon + 1)
        .checked_add(length)
        .filter(|end| *end <= input.len())
        .ok_or_else(invalid)?;
    let bytes = &input[colon + 1..end];
    *input = &input[end..];
    Ok(bytes)
}

pub(super) fn encode_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(bytes.len().to_string().as_bytes());
    output.push(b':');
    output.extend_from_slice(bytes);
}
