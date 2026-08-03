FROM mcr.microsoft.com/devcontainers/rust:1

# Install protoc (protobuf compiler), ripgrep and common utilities
RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    ripgrep \
    unzip \
    curl \
  && rm -rf /var/lib/apt/lists/*

# Ensure the workspace is available as the non-root user
USER vscode
