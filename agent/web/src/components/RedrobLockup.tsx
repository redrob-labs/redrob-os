// Redrob OS: the Redrob lockup (symbol + wordmark), shipped verbatim from the
// design system. The variant is picked by ground, never recolored: the white
// wordmark on dark and OLED, the black wordmark on light.
import { useTheme } from '@/hooks/useTheme';
import { basePath } from '@/lib/basePath';

export function RedrobLockup({ height }: { height: number }) {
  const { resolvedTheme } = useTheme();
  const file = resolvedTheme === 'light' ? 'redrob-logo-primary.png' : 'redrob-logo-primary-white.png';
  return (
    <img
      src={`${basePath}/_app/${file}`}
      alt="Redrob"
      style={{ height, width: 'auto' }}
    />
  );
}
