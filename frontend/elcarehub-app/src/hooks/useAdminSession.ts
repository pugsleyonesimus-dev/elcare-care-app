"use client";

import { useState, useCallback, useEffect } from "react";

// Admin session duration (e.g., 15 minutes)
const SESSION_TIMEOUT_MS = 15 * 60 * 1000;

export function useAdminSession() {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [lastAuthTime, setLastAuthTime] = useState<number | null>(null);

  // Check if session is still valid
  const checkSession = useCallback(() => {
    if (!lastAuthTime) return false;
    const now = Date.now();
    const isValid = now - lastAuthTime < SESSION_TIMEOUT_MS;
    if (!isValid && isAuthenticated) {
      setIsAuthenticated(false);
    }
    return isValid;
  }, [lastAuthTime, isAuthenticated]);

  // Authenticate (should ideally involve a wallet signature, but for now we'll track the time)
  const authenticate = useCallback(async () => {
    // In a production app, we might call signWithFreighter with a "Confirm Admin Session" message
    // For now, we'll simulate the authentication success
    setLastAuthTime(Date.now());
    setIsAuthenticated(true);
    return true;
  }, []);

  const logout = useCallback(() => {
    setIsAuthenticated(false);
    setLastAuthTime(null);
  }, []);

  // Auto-expire session
  useEffect(() => {
    if (!isAuthenticated) return;

    const interval = setInterval(() => {
      checkSession();
    }, 10000); // Check every 10 seconds

    return () => clearInterval(interval);
  }, [isAuthenticated, checkSession]);

  return {
    isAuthenticated,
    authenticate,
    logout,
    checkSession,
    sessionExpiresIn: lastAuthTime ? Math.max(0, SESSION_TIMEOUT_MS - (Date.now() - lastAuthTime)) : 0
  };
}
