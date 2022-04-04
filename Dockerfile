FROM rust:slim-buster as builder
WORKDIR /code

COPY . .
RUN cargo b --release \
    && strip target/release/nfo-maker

# 
FROM debian:buster-slim
WORKDIR /app
COPY --from=builder /code/target/release/nfo-maker .
ENTRYPOINT [ "./nfo-maker" ]
CMD []
