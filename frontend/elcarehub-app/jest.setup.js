import '@testing-library/jest-dom'
import 'whatwg-fetch'
import { toHaveNoViolations } from 'jest-axe'

expect.extend(toHaveNoViolations)
