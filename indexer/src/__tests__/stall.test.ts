import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockStalledGauge = vi.hoisted(() => ({ set: vi.fn() }));

vi.mock('../metrics.js', () => ({
  stalledGauge: mockStalledGauge,
  latestLedgerProcessedGauge: { set: vi.fn() },
  networkLatestLedgerGauge: { set: vi.fn() },
  syncLatencyGauge: { set: vi.fn() },
}));

import {
  recordProgress,
  isStalled,
  resetStallStateForTest,
  STALL_THRESHOLD_MS,
} from '../stall';

describe('stall detection', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    resetStallStateForTest();
    mockStalledGauge.set.mockClear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('isStalled() returns false before any progress has been recorded', () => {
    expect(isStalled()).toBe(false);
  });

  it('isStalled() returns false immediately after recordProgress()', () => {
    recordProgress();
    expect(isStalled()).toBe(false);
  });

  it('isStalled() returns true after the stall threshold elapses without progress', () => {
    recordProgress();
    vi.advanceTimersByTime(STALL_THRESHOLD_MS + 1);
    expect(isStalled()).toBe(true);
  });

  it('stalledGauge is set to 1 when the stall timer fires', () => {
    recordProgress();
    vi.advanceTimersByTime(STALL_THRESHOLD_MS + 1);
    expect(mockStalledGauge.set).toHaveBeenCalledWith(1);
  });

  it('stalledGauge is set to 0 on each recordProgress() call', () => {
    recordProgress();
    expect(mockStalledGauge.set).toHaveBeenCalledWith(0);
  });

  it('recovery: isStalled() returns false again after a new recordProgress()', () => {
    recordProgress();
    vi.advanceTimersByTime(STALL_THRESHOLD_MS + 1);
    expect(isStalled()).toBe(true);

    recordProgress(); // simulate recovery
    expect(isStalled()).toBe(false);
  });

  it('recovery: stalledGauge is cleared to 0 after recovery', () => {
    recordProgress();
    vi.advanceTimersByTime(STALL_THRESHOLD_MS + 1);

    mockStalledGauge.set.mockClear();
    recordProgress(); // recovery
    expect(mockStalledGauge.set).toHaveBeenCalledWith(0);
    expect(mockStalledGauge.set).not.toHaveBeenCalledWith(1);
  });

  it('recovery: the stall timer is cancelled on recordProgress() so gauge does not fire late', () => {
    recordProgress();
    vi.advanceTimersByTime(STALL_THRESHOLD_MS / 2);
    recordProgress(); // reset timer mid-flight
    mockStalledGauge.set.mockClear();

    // Advance past where the FIRST timer would have fired
    vi.advanceTimersByTime(STALL_THRESHOLD_MS / 2 + 1);
    expect(mockStalledGauge.set).not.toHaveBeenCalledWith(1);
  });

  it('isStalled() stays false just below the threshold', () => {
    recordProgress();
    vi.advanceTimersByTime(STALL_THRESHOLD_MS - 1);
    expect(isStalled()).toBe(false);
  });
});
