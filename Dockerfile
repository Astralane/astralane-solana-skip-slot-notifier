FROM rust:1.79 as build

WORKDIR /usr/src/app
COPY . .
RUN ls -lart
RUN cargo build --release
RUN ls -lart /usr/src/app/target/release/
RUN pwd
FROM rust:1.79

ENV API=api.testnet.solana.com
WORKDIR /home/nonrootuser
RUN groupadd -r nonrootuser && useradd -m -r -g nonrootuser nonrootuser
COPY --from=build /usr/src/app/target/release/slot_bot .
USER nonrootuser
# set the startup command to run your binary
ENTRYPOINT ["/home/nonrootuser/slot_bot"]