FROM rust:1.85-slim-bullseye

# Install dependencies: C/C++ toolchains, GHC, Python stack, and TeX Live
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    python3 \
    python3-pip \
    gcc \
    clang \
    ghc \
    texlive-latex-base \
    texlive-latex-extra \
    texlive-fonts-recommended \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Lean 4 / Lake via elan
RUN curl -sSf https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y
ENV PATH="/root/.elan/bin:${PATH}"

# Set workspace
WORKDIR /artifact
COPY . .

# Build NumLang compiler
RUN cargo build --release

# Build Lean proofs
WORKDIR /artifact/lean
RUN lake build

# Python dependencies for benchmarks
WORKDIR /artifact
RUN pip3 install --no-cache-dir tabulate scipy numpy

# Setup entrypoint
COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
