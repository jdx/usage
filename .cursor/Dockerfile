# Cloud Agent base image for jdx/usage.
#
# Provides the stable system layer: OS packages, the Rust toolchain, mise, and
# the shells the completion integration tests exercise. Repo-pinned tools
# (declared in mise.toml) are installed after checkout by .cursor/install.sh,
# because mise.toml is only available once the repository is cloned.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# Default to a UTF-8 locale. The base image boots with the C/POSIX locale, under
# which bash reports the completion cursor (COMP_POINT) in bytes rather than
# characters; usage-argv has a test that asserts character-based cutting and
# fails without UTF-8. C.UTF-8 is built into glibc, so no locale generation is
# needed.
ENV LANG=C.UTF-8 \
    LC_ALL=C.UTF-8

# System packages:
# - ca-certificates/curl/git/xz-utils/unzip: fetch + unpack toolchains and tools
# - build-essential/pkg-config/libssl-dev: compile cargo-installed tools (cargo-edit)
# - zsh/fish: shell-completion integration tests (they panic rather than skip under CI=1)
# - libicu74: required by the PowerShell runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        xz-utils \
        unzip \
        build-essential \
        pkg-config \
        libssl-dev \
        zsh \
        fish \
        libicu74 \
    && rm -rf /var/lib/apt/lists/*

# PowerShell (pwsh) from the official tarball — the fourth shell the completion
# integration tests drive. Not packaged for Ubuntu 24.04, so install from release.
RUN PWSH_VERSION=7.6.5 \
    && curl -fsSL -o /tmp/pwsh.tar.gz \
        "https://github.com/PowerShell/PowerShell/releases/download/v${PWSH_VERSION}/powershell-${PWSH_VERSION}-linux-x64.tar.gz" \
    && mkdir -p /opt/microsoft/powershell/7 \
    && tar zxf /tmp/pwsh.tar.gz -C /opt/microsoft/powershell/7 \
    && chmod +x /opt/microsoft/powershell/7/pwsh \
    && ln -sf /opt/microsoft/powershell/7/pwsh /usr/local/bin/pwsh \
    && rm -f /tmp/pwsh.tar.gz \
    && pwsh --version

# Run as the non-root `ubuntu` user (uid 1000) that ships with the base image.
# The Rust toolchain and mise live under this user's home so the agent can write
# to the cargo registry cache and install tools without permission workarounds.
USER ubuntu
ENV HOME=/home/ubuntu
ENV RUSTUP_HOME=/home/ubuntu/.rustup \
    CARGO_HOME=/home/ubuntu/.cargo \
    PATH=/home/ubuntu/.local/bin:/home/ubuntu/.cargo/bin:$PATH

# Rust toolchain via rustup. usage-lib declares MSRV 1.95; install current stable
# (which satisfies it) with clippy + rustfmt for the lint tasks.
RUN curl -fsSL https://sh.rustup.rs \
        | sh -s -- -y --profile default --default-toolchain stable \
    && rustc --version \
    && cargo --version

# mise — the repository's tool manager. The pinned tools themselves are installed
# from mise.toml at bootstrap time by .cursor/install.sh.
RUN curl -fsSL https://mise.run | sh \
    && "$HOME/.local/bin/mise" --version

# Activate mise and expose the Rust toolchain in interactive shells.
RUN printf '%s\n' \
    'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"' \
    'command -v mise >/dev/null 2>&1 && eval "$(mise activate bash)"' \
    >> "$HOME/.bashrc"
