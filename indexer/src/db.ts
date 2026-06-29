import { PrismaClient } from '@prisma/client';

const connectionLimit = parseInt(process.env.DB_CONNECTION_LIMIT || '10', 10);
const statementTimeout = parseInt(process.env.DB_STATEMENT_TIMEOUT || '30000', 10); // 30s default
const idleTimeout = parseInt(process.env.DB_IDLE_TIMEOUT || '30000', 10); // 30s default
const acquireTimeout = parseInt(process.env.DB_ACQUIRE_TIMEOUT || '10000', 10); // 10s default

const prisma = new PrismaClient({
  datasources: {
    db: {
      url: buildDatabaseUrl(),
    },
  },
});

function buildDatabaseUrl(): string {
  const baseUrl = process.env.DATABASE_URL;
  if (!baseUrl) {
    throw new Error('DATABASE_URL environment variable is required');
  }

  // Parse connection string to add pool parameters
  const url = new URL(baseUrl);

  // Prisma connection string pool parameters
  // Format: postgresql://...?connection_limit=10&pool_timeout=10
  url.searchParams.set('connection_limit', String(connectionLimit));
  url.searchParams.set('pool_timeout', String(acquireTimeout));

  return url.toString();
}

// Set statement timeout at session level
prisma.$executeRawUnsafe(`SET statement_timeout = ${statementTimeout}`).catch(err => {
  console.warn('Could not set statement_timeout:', err.message);
});

export default prisma;
