import * as Sentry from "@sentry/nextjs";

Sentry.init({
  dsn: process.env.NEXT_PUBLIC_SENTRY_DSN,

  environment: process.env.NODE_ENV,
  release:
    process.env.NEXT_PUBLIC_VERCEL_GIT_COMMIT_SHA ??
    process.env.NEXT_PUBLIC_APP_VERSION,

  tracesSampleRate: 1.0,
  debug: false,

  replaysOnErrorSampleRate: 1.0,
  replaysSessionSampleRate: 0.1,

  // Never send PII to Sentry.
  sendDefaultPii: false,

  integrations: [
    Sentry.replayIntegration({
      maskAllText: true,
      blockAllMedia: true,
    }),
  ],

  beforeSend(event, hint) {
    const error = hint.originalException;

    // Drop wallet connection cancellations — not actionable.
    if (error && typeof error === "object" && "message" in error) {
      const message = String(error.message).toLowerCase();
      if (
        message.includes("user rejected") ||
        message.includes("user cancelled") ||
        message.includes("user denied")
      ) {
        return null;
      }
    }

    return event;
  },
});
