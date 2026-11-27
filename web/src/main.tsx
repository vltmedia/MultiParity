import '@fontsource-variable/bricolage-grotesque/opsz.css';
import './styles/tokens.css';
import './styles/shell.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import App from './App';
// The theme layer comes after every page's own styles so theme settings win.
import './styles/theme.css';
import './styles/icons.css';
import { startAutoIcons } from './lib/autoIcons';
import { applySavedTheme } from './lib/theme';

applySavedTheme();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </StrictMode>,
);

startAutoIcons();
