import { openAuxiliaryWindow } from '../../src/lib/ipc/index.ts';
document.getElementById('settings').onclick = () => openAuxiliaryWindow('settings');
document.getElementById('stats').onclick = () => openAuxiliaryWindow('stats');
