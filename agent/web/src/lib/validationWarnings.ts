import { t } from './i18n';
import { trError } from './schemaText';
import type { ValidationWarning } from './api';

const WARNING_KEY_PREFIX = 'validation_warning.';

export function validationWarningMessage(warning: ValidationWarning): string {
  const key = `${WARNING_KEY_PREFIX}${warning.code}`;
  const localized = t(key);
  return localized === key ? trError(warning.message) : localized;
}
