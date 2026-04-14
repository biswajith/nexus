import { useEffect, useMemo } from 'react';
import { Provider } from '@react-spectrum/s2';
import { useUiStore } from './stores/ui-store.js';
import { AppLayout } from './components/layout/AppLayout.js';

function useResolvedColorScheme(): 'light' | 'dark' {
  const colorScheme = useUiStore((s) => s.colorScheme);

  useEffect(() => {
    const handler = () => useUiStore.getState().setColorScheme(useUiStore.getState().colorScheme);
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, []);

  return useMemo(() => {
    if (colorScheme === 'system') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }
    return colorScheme;
  }, [colorScheme]);
}

export function App() {
  const resolved = useResolvedColorScheme();

  useEffect(() => {
    document.documentElement.setAttribute('data-color-scheme', resolved);
  }, [resolved]);

  return (
    <Provider background="base" colorScheme={resolved}>
      <AppLayout />
    </Provider>
  );
}
