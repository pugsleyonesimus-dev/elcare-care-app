import { renderHook, act } from "@testing-library/react";
import { useAdminSession } from "@/hooks/useAdminSession";

describe("useAdminSession", () => {
  beforeEach(() => {
    jest.useFakeTimers();
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it("should start as unauthenticated", () => {
    const { result } = renderHook(() => useAdminSession());
    expect(result.current.isAuthenticated).toBe(false);
  });

  it("should become authenticated after calling authenticate", async () => {
    const { result } = renderHook(() => useAdminSession());
    await act(async () => {
      await result.current.authenticate();
    });
    expect(result.current.isAuthenticated).toBe(true);
  });

  it("should expire after timeout", async () => {
    const { result } = renderHook(() => useAdminSession());
    await act(async () => {
      await result.current.authenticate();
    });
    expect(result.current.isAuthenticated).toBe(true);

    // Advance time by 16 minutes (timeout is 15)
    act(() => {
      jest.advanceTimersByTime(16 * 60 * 1000);
    });

    // The checkSession is called by an interval or manually
    act(() => {
      result.current.checkSession();
    });

    expect(result.current.isAuthenticated).toBe(false);
  });

  it("should logout successfully", async () => {
    const { result } = renderHook(() => useAdminSession());
    await act(async () => {
      await result.current.authenticate();
    });
    expect(result.current.isAuthenticated).toBe(true);

    act(() => {
      result.current.logout();
    });
    expect(result.current.isAuthenticated).toBe(false);
  });
});
