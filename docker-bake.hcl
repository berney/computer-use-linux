# Set from the shell's PWD, so each worktree keeps its own cargo target cache.
variable "PWD" {
  default = ""
}

group "default" {
  targets = ["linuxserver-mod"]
}

# The mod image, loaded into the local image store.
# Publish with: docker bake --push --set linuxserver-mod.tags=<registry>/<name>:<tag>
target "linuxserver-mod" {
  context    = "."
  dockerfile = "docker/linuxserver-mod/Dockerfile"
  tags       = ["computer-use-linux-mod:dev"]
  output     = ["type=docker"]
  args = {
    CARGO_CACHE_ID = PWD == "" ? "shared" : "${basename(PWD)}-${substr(sha1(PWD), 0, 8)}"
  }
}

# The same files as a tarball for DOCKER_MODS_SIDELOAD: mount it at /mods/computer-use-linux.tar.
target "linuxserver-mod-tar" {
  inherits = ["linuxserver-mod"]
  tags     = []
  output   = ["type=tar,dest=dist/computer-use-linux.tar"]
}
