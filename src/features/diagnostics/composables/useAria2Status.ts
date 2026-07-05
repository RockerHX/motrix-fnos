import { ref } from "vue";
import { getAria2ProcessStatus, pingAria2Rpc } from "../../../services/aria2";
import type { Aria2ProcessStatus, Aria2RpcStatus } from "../../../types/aria2";

export type Aria2StatusSnapshot = {
  process: Aria2ProcessStatus;
  rpc: Aria2RpcStatus;
};

export function useAria2Status() {
  const aria2Process = ref<Aria2ProcessStatus | null>(null);
  const aria2Rpc = ref<Aria2RpcStatus | null>(null);

  async function refreshAria2Status() {
    const [process, rpc] = await Promise.all([getAria2ProcessStatus(), pingAria2Rpc()]);
    aria2Process.value = process;
    aria2Rpc.value = rpc;
  }

  function updateAria2Status(status: Aria2StatusSnapshot) {
    aria2Process.value = status.process;
    aria2Rpc.value = status.rpc;
  }

  return {
    aria2Process,
    aria2Rpc,
    refreshAria2Status,
    updateAria2Status,
  };
}
