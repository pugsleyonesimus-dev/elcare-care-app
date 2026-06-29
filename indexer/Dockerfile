# ── Stage 1: builder ─────────────────────────────────────────────────────────
FROM node:20-alpine AS builder

WORKDIR /build

# Install openssl (needed by Prisma at generate time)
RUN apk add --no-cache openssl

# Install dependencies (including devDeps for TypeScript compilation)
COPY package*.json ./
RUN npm ci

COPY tsconfig.json ./
COPY prisma ./prisma/

# Generate Prisma client for the target platform
RUN npx prisma generate

COPY src ./src/

# Compile TypeScript
RUN npm run build

# ── Stage 2: runtime ─────────────────────────────────────────────────────────
FROM node:20-alpine AS runtime

WORKDIR /app

# Runtime system deps
RUN apk add --no-cache openssl \
 && addgroup -S appgroup \
 && adduser  -S appuser -G appgroup

# Install production-only deps
COPY package*.json ./
RUN npm ci --omit=dev

# Copy compiled output and Prisma artifacts
COPY --from=builder /build/dist        ./dist
COPY --from=builder /build/prisma      ./prisma
COPY --from=builder /build/node_modules/.prisma ./node_modules/.prisma

EXPOSE 4000

HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=3 \
  CMD wget -qO- http://localhost:4000/health || exit 1

USER appuser

CMD ["sh", "-c", "npx prisma migrate deploy && node dist/index.js"]
