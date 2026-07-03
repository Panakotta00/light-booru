FROM rust

WORKDIR /app/src
COPY . .

RUN apt update && apt-get install -y pkg-config libgexiv2-dev nodejs npm
RUN npm install -g pnpm@9
RUN pnpm install && pnpm run tailwind
RUN cargo install --path .

CMD ["light-booru"]