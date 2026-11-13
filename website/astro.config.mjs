import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';
const origin='https://itsmequote.github.io';
export default defineConfig({site:origin,base:'/epholang',trailingSlash:'always',integrations:[sitemap({serialize(item){const path=new URL(item.url).pathname;const suffix=path.replace(/^\/epholang\/(ru|en)\//,'');const root=path==='/epholang/';item.links=[{lang:'ru',url:origin+'/epholang/ru/'+(root?'':suffix)},{lang:'en',url:origin+'/epholang/en/'+(root?'':suffix)}];if(root)item.links.push({lang:'x-default',url:origin+'/epholang/'});return item;}})]});
