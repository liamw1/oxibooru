# BUILDER IMAGE
FROM oven/bun:1 AS builder

# Create app directory
WORKDIR /app

# Get app dependencies
COPY package.json bun.lock ./

# building app
RUN bun install

# Bundle app source
COPY . .

# Build app
RUN bun run build

# FINAL IMAGE
FROM oven/bun:1 AS final

# Create app directory
WORKDIR /usr/src/app

# Get app dependencies. Needed because of module imports
COPY package.json bun.lock ./

# Copy build result
COPY --from=builder /app/build .

# set startup command
CMD ["bun", "run", "index.js"]