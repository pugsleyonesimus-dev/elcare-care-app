import { z } from 'zod';
import { Request, Response, NextFunction } from 'express';
import { badRequest } from './errors.js';

// ── Reusable field schemas ────────────────────────────────────────────────────

const positiveInt = (max: number) =>
  z.coerce
    .number()
    .int()
    .nonnegative()
    .max(max);

const optionalString = z.string().optional();

// ── Per-endpoint schemas ──────────────────────────────────────────────────────

export const listingsQuerySchema = z.object({
  artist:   optionalString,
  owner:    optionalString,
  status:   optionalString,
  search:   optionalString,
  minPrice: z.coerce.number().nonnegative().optional(),
  maxPrice: z.coerce.number().nonnegative().optional(),
  limit:    positiveInt(1000).optional(),
  offset:   positiveInt(10_000).optional(),
});

export const auctionsQuerySchema = z.object({
  creator: optionalString,
  status:  optionalString,
});

export const offersQuerySchema = z.object({
  listing_id: z
    .string()
    .regex(/^\d+$/, 'listing_id must be a non-negative integer')
    .optional(),
});

export const walletActivityQuerySchema = z.object({
  limit: positiveInt(200).optional(),
});

export const collectionsQuerySchema = z.object({
  kind:    optionalString,
  creator: optionalString,
});

export const statsQuerySchema = z.object({
  range: z.enum(['day', 'week', 'month']).optional(),
  from:  z.string().optional(),
  to:    z.string().optional(),
});

// ── Analytics dashboard schemas ───────────────────────────────────────────────

// ISO 8601 date string validator (YYYY-MM-DD or full ISO timestamp)
const isoDateString = z
  .string()
  .refine((s) => !isNaN(Date.parse(s)), {
    message: 'Must be a valid ISO 8601 date string',
  });

export const statsOverviewQuerySchema = z.object({});

export const statsDailyQuerySchema = z.object({
  from: isoDateString,
  to:   isoDateString,
});

export const statsTopQuerySchema = z.object({
  limit: z.coerce.number().int().positive().max(100).optional(),
});

// ── Middleware factory ────────────────────────────────────────────────────────

export function validateQuery<T extends z.ZodTypeAny>(schema: T) {
  return (req: Request, _res: Response, next: NextFunction) => {
    const result = schema.safeParse(req.query);
    if (!result.success) {
      const message = result.error.issues
        .map((e) => `${e.path.join('.')}: ${e.message}`)
        .join('; ');
      return next(badRequest(message));
    }
    // Replace raw query with coerced, validated values
    (req as any).validatedQuery = result.data;
    next();
  };
}
