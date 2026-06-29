import { Request, Response, NextFunction } from 'express';

export const ErrorCode = {
  BAD_REQUEST: 'BAD_REQUEST',
  NOT_FOUND: 'NOT_FOUND',
  INTERNAL: 'INTERNAL_SERVER_ERROR',
} as const;

export type ErrorCode = (typeof ErrorCode)[keyof typeof ErrorCode];

export class ApiError extends Error {
  constructor(
    public readonly statusCode: number,
    public readonly code: ErrorCode,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export function badRequest(message: string): ApiError {
  return new ApiError(400, ErrorCode.BAD_REQUEST, message);
}

export function notFound(message: string): ApiError {
  return new ApiError(404, ErrorCode.NOT_FOUND, message);
}

export function internalError(message = 'An unexpected error occurred'): ApiError {
  return new ApiError(500, ErrorCode.INTERNAL, message);
}

// Maps known domain errors to their ApiError equivalent.
function toApiError(err: unknown): ApiError {
  if (err instanceof ApiError) return err;
  return internalError();
}

export function errorHandler(
  err: unknown,
  _req: Request,
  res: Response,
  _next: NextFunction,
): void {
  const apiErr = toApiError(err);

  if (apiErr.statusCode >= 500) {
    console.error('[ErrorHandler]', err);
  }

  res.status(apiErr.statusCode).json({
    error: {
      code: apiErr.code,
      message: apiErr.message,
    },
  });
}
