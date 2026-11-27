import { TERM, cd, cmd, edit, out, lastCmd } from './term.mjs';

// Real output from these commands against the demo server, shown with its studio address.
export default async function scene(r) {
  await r.open(TERM, 700);
  await cd(r, '~/build/Emberfall');
  await r.park(1100, 640, 10);
  await r.record();

  await r.beat('hello');
  await r.park(1000, 600, 1200);

  await r.beat('login');
  await cmd(r, 'mp login https://mp.cinderworks.studio', 16);
  await out(r, ['Email: build@cinderworks.studio', 'Password: ', 'Signed in as build@cinderworks.studio on https://mp.cinderworks.studio.'], 300);
  await cmd(r, 'mp get-latest', 14);
  await out(r, ['  getting Characters/EmberWolf/ember_wolf_fur.png', '  getting Characters/Hero/hero_diffuse.png', '  getting Environments/Forge/forge_floor_stone.png', 'Up to date with change 13. 3 downloaded (533 KB), 0 removed.'], 140);

  await r.beat('aliases');
  await cmd(r, 'mp sync', 14);
  await out(r, ['Up to date with change 13. 0 downloaded (0 B), 0 removed.'], 100);
  await r.callout('code', lastCmd, { label: 'sync = get latest', dur: 2.4 });
  await cmd(r, 'mp log --json -n 1', 16);
  await out(r, ['[', '  {', '    "author": { "name": "Mara Quinn", "email": "mara@cinderworks.studio" },', '    "kind": "publish",', '    "message": "Ember wolf fur, softer underbelly",', '    "number": 13,', '    "file_count": 1,', '    "size": 241264', '  }', ']'], 50);
  await r.callout('code', lastCmd, { label: 'For your scripts', dur: 2.4 });

  await r.beat('mcp');
  await edit(r, 'claude_desktop_config.json', `{
  "mcpServers": {
    "multiparity": {
      "command": "mp",
      "args": ["mcp"]
    }
  }
}`);
  await r.callout('code', { css: '#L5' }, { label: 'mp mcp', dur: 3.4 });
  await r.park(900, 500, 900);
  await r.end(1.8);
}
scene.options = { view: 1440 };
