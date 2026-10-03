import { readFile } from 'node:fs/promises';
import { mergeConfig } from 'vite';
import baseConfig from '../../vite.config.js';

export default async (env) =>
  mergeConfig(await baseConfig(env), {
    plugins: [
      {
        name: 'mini-test-fixture',
        configureServer(server) {
          server.middlewares.use('/__mini_fixture', async (_request, response, next) => {
            try {
              response.setHeader('Content-Type', 'text/html');
              response.end(
                await server.transformIndexHtml(
                  '/__mini_fixture',
                  await readFile(new URL('./preview.html', import.meta.url), 'utf8')
                )
              );
            } catch (error) {
              next(error);
            }
          });
        },
      },
    ],
  });
