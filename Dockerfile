# --- Build --- #

FROM rust:1.94-slim-bookworm AS builder

# Needed for git tag displaying in the bot
RUN apt-get update \
    && apt-get install -y git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
COPY . .


RUN cargo build --release



# --- Run --- #

FROM debian:bookworm-slim

WORKDIR /app


# Installing OpenSSL and CA certificates (required for Discord API/HTTPS)
RUN apt-get update \
    && apt-get install -y ca-certificates openssl \
    && rm -rf /var/lib/apt/lists/*


COPY --from=builder /usr/src/app/target/release/ccs_discord_bot /app/ccs_discord_bot


# Creating a non-privileged user for security
RUN groupadd -r botgroup \
    && useradd -r -g botgroup botuser \
    && chown -R botuser:botgroup /app \
    && chmod +x /app/ccs_discord_bot

# Switching to the non-privileged user
USER botuser

WORKDIR /app/bot_data

CMD ["/app/ccs_discord_bot"]
