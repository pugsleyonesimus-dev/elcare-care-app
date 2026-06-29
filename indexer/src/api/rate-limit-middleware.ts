import rateLimit from 'express-rate-limit';

// Global baseline limiter — applies to all public endpoints
const GLOBAL_LIMIT = parseInt(process.env.RATE_LIMIT_GLOBAL || '500');
const GLOBAL_WINDOW_MS = 60 * 1000; // 1 minute

/**
 * Global baseline rate limiter — 500 requests per minute (configurable)
 */
export const globalRateLimiter = rateLimit({
    windowMs: GLOBAL_WINDOW_MS,
    max: GLOBAL_LIMIT,
    message: {
        error: 'Too many requests, please try again later.',
        retryAfter: '1 minute'
    },
    standardHeaders: true,
    legacyHeaders: false,
    skip: (req) => req.path === '/health' || req.path === '/readyz',
});

/**
 * Standard rate limiter — 100 requests per minute
 */
export const rateLimiter = rateLimit({
    windowMs: 60 * 1000, // 1 minute
    max: 100, // Limit each IP to 100 requests per windowMs
    message: {
        error: 'Too many requests from this IP, please try again later.',
        retryAfter: '1 minute'
    },
    standardHeaders: true, // Return rate limit info in the `RateLimit-*` headers
    legacyHeaders: false, // Disable the `X-RateLimit-*` headers
    skip: (req) => req.path === '/health' || req.path === '/readyz',
});

/**
 * Stricter rate limiter for resource-intensive endpoints
 * 20 requests per minute for endpoints that query large datasets (search, history, activity)
 */
export const strictRateLimiter = rateLimit({
    windowMs: 60 * 1000, // 1 minute
    max: parseInt(process.env.RATE_LIMIT_STRICT || '20'),
    message: {
        error: 'Too many requests to this endpoint, please try again later.',
        retryAfter: '1 minute'
    },
    standardHeaders: true,
    legacyHeaders: false,
});
