import * as Sentry from "@sentry/nextjs";

Sentry.init({
  dsn: process.env.NEXT_PUBLIC_SENTRY_DSN,

  environment: process.env.NODE_ENV,
  release:
    process.env.NEXT_PUBLIC_VERCEL_GIT_COMMIT_SHA ??
    process.env.NEXT_PUBLIC_APP_VERSION,

  tracesSampleRate: 1.0,
  debug: false,

  // Never send PII to Sentry.
  sendDefaultPii: false,

  beforeSend(event) {
    // Strip any accidentally captured authorization / cookie headers.
    if (event.request?.headers) {
      const { authorization, cookie, ...safeHeaders } = event.request.headers as Record<string, string>;
      void authorization;
      void cookie;
      event.request.headers = safeHeaders;
    }
    return event;
  },
});
