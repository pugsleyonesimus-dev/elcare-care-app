import { Request, Response, NextFunction } from 'express';
import crypto from 'crypto';

export function etagMiddleware(req: Request, res: Response, next: NextFunction) {
  const originalJson = res.json.bind(res);

  res.json = function(data: any) {
    const payload = JSON.stringify(data);
    const etag = `"${crypto.createHash('md5').update(payload).digest('hex')}"`;
    
    res.set('ETag', etag);
    
    if (req.get('If-None-Match') === etag) {
      return res.status(304).end();
    }

    return originalJson(data);
  };

  next();
}
