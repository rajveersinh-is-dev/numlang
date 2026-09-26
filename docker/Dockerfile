FROM debian:bookworm-slim AS base

# Prevent interactive prompts during apt install
ENV DEBIAN_FRONTEND=noninteractive

# Install core build dependencies, C/C++ toolchains, GHC Haskell, Python scientific stack
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    git \
    llvm-17 \
    llvm-17-dev \
    clang-17 \
    libclang-17-dev \
    zlib1g-dev \
    python3 \
    python3-pip \
    python3-matplotlib \
    python3-scipy \
    python3-pandas \
    python3-numpy \
    ghc \
    cabal-install \
    ca-certificates \
    texlive-latex-base \
    texlive-latex-extra \
    texlive-fonts-recommended \
    latexmk \
    && rm -rf /var/lib/apt/lists/*

# Symlink clang-17 to clang and clang++
RUN ln -s /usr/bin/clang-17 /usr/bin/clang && \
    ln -s /usr/bin/clang++-17 /usr/bin/clang++

# Install pinned Rust toolchain (1.81.0)
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.81.0

# Install Lean 4 via elan (v4.11.0)
ENV ELAN_HOME=/usr/local/elan \
    PATH=/usr/local/elan/bin:$PATH
RUN curl -sSfL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y --default-toolchain leanprover/lean4:v4.11.0

# Set up project workspace
WORKDIR /numlang
COPY . .

# Build NumLang compiler
RUN cargo build --release --all-features

# Verify Lean 4 proofs
WORKDIR /numlang/proof
RUN lake build

# Return to root workspace
WORKDIR /numlang

# Make scripts executable
RUN chmod +x scripts/*.sh

# Default container entrypoint executes all verification gates and benchmark suites
CMD ["bash", "scripts/run_all_experiments.sh"]
