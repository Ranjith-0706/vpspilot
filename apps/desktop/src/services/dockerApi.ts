import type { DockerResult } from "../types/docker";
import { call } from "./ipc";

export const dockerApi = {
  /**
   * Lists a saved server's containers. Read-only: the backend picks the host, port and
   * credential itself, runs one fixed `docker ps` command, and refuses unless the server's host
   * key is already trusted. Never uses sudo.
   */
  listContainers: (serverId: string) =>
    call<DockerResult>("list_docker_containers", { id: serverId }),
};
