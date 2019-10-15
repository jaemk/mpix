# need to build with nightly until async/await is stable
# since there's no beta images
FROM rustlang/rust:nightly

# create a new empty shell of the target project
RUN USER=root cargo new --bin mpix
WORKDIR /mpix

#COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# build and cached dependencies
RUN cargo build --release

# ---------------------------------------------

# clear out the placeholder source code
RUN rm -f ./src/*.rs

# copy over target source files
COPY ./src ./src

# copy over git dir and embed latest commit hash
COPY ./.git ./.git
COPY ./commit_hash.txt ./commit_hash.txt
# make sure there's no trailing newline
RUN git rev-parse HEAD | awk '{ printf "%s", $0 >"commit_hash.txt" }'

RUN rm -rf ./.git

# build for release
RUN rm -f ./target/release/deps/mpix*
RUN cargo build --release

CMD ["./target/release/mpix"]
