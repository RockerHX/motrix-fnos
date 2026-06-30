import { invoke } from "@tauri-apps/api/core";
import type { Aria2ConfigStatus, Aria2ProcessStatus, Aria2RpcStatus } from "../types/aria2";

export function getAria2ConfigStatus(): Promise<Aria2ConfigStatus> {
  return invoke<Aria2ConfigStatus>("get_aria2_config_status");
}

export function getAria2ProcessStatus(): Promise<Aria2ProcessStatus> {
  return invoke<Aria2ProcessStatus>("get_aria2_process_status");
}

export function startAria2(): Promise<Aria2ProcessStatus> {
  return invoke<Aria2ProcessStatus>("start_aria2");
}

export function stopAria2(): Promise<Aria2ProcessStatus> {
  return invoke<Aria2ProcessStatus>("stop_aria2");
}

export function pingAria2Rpc(): Promise<Aria2RpcStatus> {
  return invoke<Aria2RpcStatus>("ping_aria2_rpc");
}
