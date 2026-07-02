import { httpGet, httpPost } from "./http";
import type { Aria2ConfigStatus, Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";

export function getAria2ConfigStatus(): Promise<Aria2ConfigStatus> {
  return httpGet<Aria2ConfigStatus>("/api/aria2/config");
}

export function getAria2ProcessStatus(): Promise<Aria2ProcessStatus> {
  return httpGet<Aria2ProcessStatus>("/api/aria2/process");
}

export function startAria2(): Promise<Aria2ProcessStatus> {
  return httpPost<Aria2ProcessStatus>("/api/aria2/start");
}

export function stopAria2(): Promise<Aria2ProcessStatus> {
  return httpPost<Aria2ProcessStatus>("/api/aria2/stop");
}

export function pingAria2Rpc(): Promise<Aria2RpcStatus> {
  return httpGet<Aria2RpcStatus>("/api/aria2/rpc");
}
