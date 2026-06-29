import { Request, Response, NextFunction } from 'express';
import redisClient from '../redis.js';

function isRedisReady(client: any) {
    if (typeof client?.isReady === 'boolean') {
        return client.isReady;
    }

    if (typeof client?.status === 'string') {
        return client.status === 'ready';
    }

    return Boolean(client?.isOpen);
}

/**
 * Cache middleware with TTL support
 * @param ttl Time-to-live in seconds
 */
export const cacheMiddleware = (ttl: number) => {
    return async (req: Request, res: Response, next: NextFunction) => {
        // Skip caching if Redis is not connected
        const client = redisClient as any;
        if (!isRedisReady(client)) {
            return next();
        }

        const cacheKey = `cache:${req.originalUrl || req.url}`;

        try {
            const cachedData = await redisClient.get(cacheKey);

            if (cachedData) {
                return res.json(JSON.parse(cachedData));
            }

            const originalJson = res.json.bind(res);

            res.json = function (data: any) {
                client.setEx(cacheKey, ttl, JSON.stringify(data)).catch((err: unknown) => {
                    console.error('Failed to cache data:', err);
                });
                return originalJson(data);
            };

            next();
        } catch (err: unknown) {
            console.error('Cache middleware error:', err);
            next();
        }
    };
};

/**
 * Invalidate cache keys matching a pattern
 * @param pattern Cache key pattern (e.g., "cache:*listing:123*")
 */
export async function invalidateCache(pattern: string): Promise<void> {
    const client = redisClient as any;
    if (!isRedisReady(client)) {
        return;
    }

    try {
        const keys = await client.keys(pattern);
        if (keys.length > 0) {
            await client.del(keys);
        }
    } catch (err: unknown) {
        console.error(`Failed to invalidate cache pattern ${pattern}:`, err);
    }
}

/**
 * Invalidate cache for a resource by ID
 * @param resourceType Type of resource (e.g., "listing", "auction")
 * @param resourceId ID of the resource
 */
export async function invalidateCacheForResource(resourceType: string, resourceId: string | number): Promise<void> {
    await invalidateCache(`cache:*${resourceType}:${resourceId}*`);
}
