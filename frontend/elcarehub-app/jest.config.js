const nextJest = require('next/jest')

const createJestConfig = nextJest({
  dir: './',
})

const customJestConfig = {
  setupFilesAfterEnv: ['<rootDir>/jest.setup.js'],
  testEnvironment: 'jest-environment-jsdom',
  testPathIgnorePatterns: ['<rootDir>/e2e/', '<rootDir>/tests/e2e/'],
  moduleNameMapper: {
    '^@/(.*)$': '<rootDir>/src/$1',
  },
  collectCoverage: process.env.CI === 'true' || process.env.COLLECT_COVERAGE === 'true',
  collectCoverageFrom: [
    'src/**/*.{js,jsx,ts,tsx}',
    '!src/**/*.d.ts',
    '!src/**/__tests__/**',
    '!src/app/**/layout.tsx',
    '!src/app/**/loading.tsx',
    '!src/app/**/error.tsx',
    '!src/app/**/not-found.tsx',
  ],
  coverageDirectory: 'coverage',
  coverageReporters: ['text', 'text-summary', 'lcov', 'json-summary'],
  coverageThreshold: {
    global: {
      statements: 60,
      branches: 50,
      functions: 55,
      lines: 60,
    },
    './src/components/CheckoutModal.tsx': {
      statements: 90,
      branches: 75,
      functions: 85,
      lines: 90,
    },
    './src/components/ListingCard.tsx': {
      statements: 90,
      branches: 75,
      functions: 85,
      lines: 90,
    },
    './src/hooks/useMarketplace.ts': {
      statements: 55,
      branches: 45,
      functions: 50,
      lines: 55,
    },
    './src/lib/contract.ts': {
      statements: 15,
      branches: 10,
      functions: 10,
      lines: 15,
    },
  },
}

module.exports = createJestConfig(customJestConfig)
