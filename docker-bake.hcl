target "linuxserver-mod" {
  context    = "."
  dockerfile = "docker/linuxserver-mod/Dockerfile"
  tags       = ["computer-use-linux-mod:dev"]
}
