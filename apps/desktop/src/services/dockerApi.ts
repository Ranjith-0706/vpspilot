import type { DockerResult, LogsResult } from "../types/docker";
import { call } from "./ipc";

export const dockerApi = {
  /**
   * Lists a saved server's containers. Read-only: the backend picks the host, port and
   * credential itself, runs one fixed `docker ps` command, and refuses unless the server's host
   * key is already trusted. Never uses sudo.
   */
  listContainers: (serverId: string) =>
    call<DockerResult>("list_docker_containers", { id: serverId }),

  /**
   * Recent logs of one container. The backend accepts only a container id (12 to 64 lowercase hex
   * characters) and a line count from 1 to 1000, so the request cannot change what runs.
   */
  getLogs: (serverId: string, containerId: string, tail: number) =>
    call<LogsResult>("get_container_logs", { id: serverId, containerId, tail }),
};
