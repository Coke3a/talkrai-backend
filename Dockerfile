# Stage 1: Chef - base image with cargo-chef installed
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
RUN apt-get update && apt-get install -y libpq-dev pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Stage 2: Planner - compute the recipe (dependency lock)
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Deps - build only dependencies (cached layer)
FROM chef AS deps
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 4: Build - compile the actual application
FROM deps AS build
COPY . .
RUN cargo build --release --bin talkrai-backend

# Stage 5: Runtime - minimal production image
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Asia/Bangkok

RUN groupadd -g 10001 app && \
    useradd -u 10001 -g app -s /bin/false app

COPY --from=build /app/target/release/talkrai-backend /usr/local/bin/talkrai-backend

USER app

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/talkrai-backend"]
