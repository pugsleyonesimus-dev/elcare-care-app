/**
 * @jest-environment node
 *
 * Run in Node (not jsdom) so that typeof window === "undefined" holds,
 * matching the server-side startup path where all required vars are checked.
 */

import { assertConfig } from "@/lib/config";

const ALL_REQUIRED: Record<string, string> = {
  NEXT_PUBLIC_CONTRACT_ID: "C_MARKETPLACE",
  NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID: "C_LAUNCHPAD",
  PINATA_JWT: "pj_test_secret",
};

describe("assertConfig", () => {
  const originalEnv = process.env;

  beforeEach(() => {
    process.env = { ...originalEnv, ...ALL_REQUIRED };
  });

  afterEach(() => {
    process.env = originalEnv;
  });

  it("does not throw when all required variables are set", () => {
    expect(() => assertConfig()).not.toThrow();
  });

  it("throws when NEXT_PUBLIC_CONTRACT_ID is missing", () => {
    delete process.env.NEXT_PUBLIC_CONTRACT_ID;
    expect(() => assertConfig()).toThrow("NEXT_PUBLIC_CONTRACT_ID");
  });

  it("throws when NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID is missing", () => {
    delete process.env.NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID;
    expect(() => assertConfig()).toThrow("NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID");
  });

  it("throws when PINATA_JWT is missing on the server", () => {
    delete process.env.PINATA_JWT;
    expect(() => assertConfig()).toThrow("PINATA_JWT");
  });

  it("throws an aggregated error listing every missing variable", () => {
    delete process.env.NEXT_PUBLIC_CONTRACT_ID;
    delete process.env.NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID;
    let caught: Error | null = null;
    try {
      assertConfig();
    } catch (e) {
      caught = e as Error;
    }
    expect(caught).not.toBeNull();
    expect(caught!.message).toContain("NEXT_PUBLIC_CONTRACT_ID");
    expect(caught!.message).toContain("NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID");
  });

  it("does not throw for PINATA_JWT when running in a browser (window defined)", () => {
    delete process.env.PINATA_JWT;
    // Simulate browser environment by defining window on global
    (global as Record<string, unknown>).window = {};
    try {
      expect(() => assertConfig()).not.toThrow();
    } finally {
      delete (global as Record<string, unknown>).window;
    }
  });
});
