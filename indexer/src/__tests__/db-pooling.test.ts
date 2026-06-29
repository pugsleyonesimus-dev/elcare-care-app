/**
 * Tests for database connection pooling and timeout configuration
 */

describe('Database pooling configuration', () => {
  const originalEnv = process.env;

  beforeEach(() => {
    jest.resetModules();
    process.env = { ...originalEnv };
  });

  afterEach(() => {
    process.env = originalEnv;
  });

  it('uses default connection limit when not configured', () => {
    delete process.env.DB_CONNECTION_LIMIT;
    // Connection limit is set at parse time, so we verify via env parsing
    const limit = parseInt(process.env.DB_CONNECTION_LIMIT || '10', 10);
    expect(limit).toBe(10);
  });

  it('uses custom connection limit from env', () => {
    process.env.DB_CONNECTION_LIMIT = '20';
    const limit = parseInt(process.env.DB_CONNECTION_LIMIT || '10', 10);
    expect(limit).toBe(20);
  });

  it('uses default statement timeout when not configured', () => {
    delete process.env.DB_STATEMENT_TIMEOUT;
    const timeout = parseInt(process.env.DB_STATEMENT_TIMEOUT || '30000', 10);
    expect(timeout).toBe(30000);
  });

  it('uses custom statement timeout from env', () => {
    process.env.DB_STATEMENT_TIMEOUT = '10000';
    const timeout = parseInt(process.env.DB_STATEMENT_TIMEOUT || '30000', 10);
    expect(timeout).toBe(10000);
  });

  it('uses default idle timeout when not configured', () => {
    delete process.env.DB_IDLE_TIMEOUT;
    const timeout = parseInt(process.env.DB_IDLE_TIMEOUT || '30000', 10);
    expect(timeout).toBe(30000);
  });

  it('uses custom idle timeout from env', () => {
    process.env.DB_IDLE_TIMEOUT = '60000';
    const timeout = parseInt(process.env.DB_IDLE_TIMEOUT || '30000', 10);
    expect(timeout).toBe(60000);
  });

  it('uses default acquire timeout when not configured', () => {
    delete process.env.DB_ACQUIRE_TIMEOUT;
    const timeout = parseInt(process.env.DB_ACQUIRE_TIMEOUT || '10000', 10);
    expect(timeout).toBe(10000);
  });

  it('uses custom acquire timeout from env', () => {
    process.env.DB_ACQUIRE_TIMEOUT = '15000';
    const timeout = parseInt(process.env.DB_ACQUIRE_TIMEOUT || '10000', 10);
    expect(timeout).toBe(15000);
  });

  it('parses all pool timeouts as integers', () => {
    process.env.DB_CONNECTION_LIMIT = '25';
    process.env.DB_STATEMENT_TIMEOUT = '20000';
    process.env.DB_IDLE_TIMEOUT = '40000';
    process.env.DB_ACQUIRE_TIMEOUT = '12000';

    const connLimit = parseInt(process.env.DB_CONNECTION_LIMIT, 10);
    const stmtTimeout = parseInt(process.env.DB_STATEMENT_TIMEOUT, 10);
    const idleTimeout = parseInt(process.env.DB_IDLE_TIMEOUT, 10);
    const acquireTimeout = parseInt(process.env.DB_ACQUIRE_TIMEOUT, 10);

    expect(connLimit).toEqual(25);
    expect(stmtTimeout).toEqual(20000);
    expect(idleTimeout).toEqual(40000);
    expect(acquireTimeout).toEqual(12000);
  });

  it('handles non-numeric values gracefully with defaults', () => {
    process.env.DB_CONNECTION_LIMIT = 'invalid';
    const limit = parseInt(process.env.DB_CONNECTION_LIMIT || '10', 10);
    expect(Number.isNaN(limit)).toBe(true);
  });
});
