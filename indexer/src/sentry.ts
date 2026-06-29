import * as Sentry from '@sentry/node';

/**
 * Initialise Sentry for the indexer process.
 * This is a no-op when SENTRY_DSN is not set, so unmonitored environments
 * (local dev, CI) are completely unaffected.
 *
 * Must be called before the Express app is constructed so that Sentry can
 * instrument the framework automatically.
 */
export function initSentry(): void {
  const dsn = process.env.SENTRY_DSN;
  if (!dsn) return;

  Sentry.init({
    dsn,
    environment: process.env.NODE_ENV ?? 'development',
    release: process.env.SENTRY_RELEASE ?? process.env.npm_package_version,
    tracesSampleRate: process.env.NODE_ENV === 'production' ? 0.2 : 1.0,
    sendDefaultPii: false,
  });

  // Capture unhandled rejections and uncaught exceptions before Node terminates.
  process.on('unhandledRejection', (reason) => {
    Sentry.captureException(reason);
  });

  process.on('uncaughtException', (err) => {
    Sentry.captureException(err);
    // Re-throw after flushing so the process exits with a non-zero code.
    process.exit(1);
  });
}

export { Sentry };
