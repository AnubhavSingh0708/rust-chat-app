# ==============================
# Stage 1: Builder
# ==============================
FROM rust:slim-bookworm AS builder

# Install dependencies required for SQLite and compiling C-libs
RUN apt-get update && apt-get install -y pkg-config libssl-dev libsqlite3-dev

# Set the working directory
WORKDIR /usr/src/app

# Copy the actual code
COPY . .

# Build the application in release mode
RUN cargo build --release

# ==============================
# Stage 2: Runtime
# ==============================
FROM debian:bookworm-slim

# Install SQLite runtime libraries and CA certificates
RUN apt-get update && apt-get install -y ca-certificates libsqlite3-0 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Create necessary directories for the app to function
RUN mkdir -p /app/static /app/uploads/pfps /app/uploads/attachments /app/data

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/rust_chat_app /app/rust_chat_app

# Copy the static frontend files
COPY ./static /app/static

# Expose the web server port
EXPOSE 8080

# Run the binary
CMD ["./rust_chat_app"]