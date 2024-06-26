FROM rust:1.79-slim-buster as build

WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

FROM alpine:3.20

ENV API=api.testnet.solana.com
RUN addgroup -S nonrootuser && adduser -S nonrootuser -G nonrootuser
WORKDIR /home/nonrootuser

COPY --from=build /usr/src/app/target/release/astralane-slot-skip-bot .
USER nonrootuser
# set the startup command to run your binary
ENTRYPOINT ["/home/nonrootuser/astralane-slot-skip-bot"]