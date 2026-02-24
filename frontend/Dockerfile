# Build stage
FROM oven/bun:1 AS builder
WORKDIR /app
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile
COPY . .
ARG VITE_API_BASE_URL=/api
RUN VITE_API_BASE_URL=${VITE_API_BASE_URL} bunx vite build

# Serve stage
FROM nginxinc/nginx-unprivileged:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 8080
