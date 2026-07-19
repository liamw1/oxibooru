# Used for pushing exact-version images.
#
# Usage: docker buildx bake --push

group "default" {
  targets = ["client", "server"]
}

target "client" {
  context = "./client"
  platforms = ["linux/amd64", "linux/arm64"]
  tags = ["oxibooru/client:0.8.0"]
}

target "server" {
  context = "./server"
  platforms = ["linux/amd64", "linux/arm64"]
  tags = ["oxibooru/server:0.8.0"]
}