import { readFile } from 'node:fs/promises';
import { mergeConfig } from 'vite';
import statsFixture from '../stats/vite.config.mjs';

export default async (env) =>
  mergeConfig(await statsFixture(env), {
    plugins: [
      {
        name: 'auxiliary-native-test-host',
        configureServer(server) {
          server.middlewares.use('/__aux_native_host', async (_request, response, next) => {
            try {
              response.setHeader('Content-Type', 'text/html');
              response.end(
                await server.transformIndexHtml(
                  '/__aux_native_host',
                  await readFile(new URL('./host.html', import.meta.url), 'utf8')
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
