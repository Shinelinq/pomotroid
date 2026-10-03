import { readFile } from 'node:fs/promises';
import { mergeConfig } from 'vite';
import baseConfig from '../../vite.config.js';

export default async (env) =>
  mergeConfig(await baseConfig(env), {
    plugins: [
      {
        name: 'plans-test-fixture',
        configureServer(server) {
          server.middlewares.use('/__plans_fixture', async (_request, response, next) => {
            try {
              const html = await readFile(new URL('./preview.html', import.meta.url), 'utf8');
              response.setHeader('Content-Type', 'text/html');
              response.end(await server.transformIndexHtml('/__plans_fixture', html));
            } catch (error) {
              next(error);
            }
          });
        },
      },
    ],
  });
