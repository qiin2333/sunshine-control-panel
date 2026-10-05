import DOMPurify from 'dompurify'
import MarkdownIt from 'markdown-it'

const markdown = new MarkdownIt({
  html: true,
  breaks: true,
  linkify: true,
  validateLink: (url) => /^https?:\/\//i.test(url),
})

// Markdown links and links from raw Release HTML must leave the WebView in a
// separate, non-opener browsing context. The hook runs after DOMPurify's
// allowlist has removed unsafe attributes and only adds inert link attributes.
let linkHookInstalled = false
const installLinkHook = () => {
  if (linkHookInstalled) return
  DOMPurify.addHook('afterSanitizeAttributes', (node) => {
    if (node.nodeName === 'A') {
      node.setAttribute('target', '_blank')
      node.setAttribute('rel', 'noopener noreferrer')
    }
  })
  linkHookInstalled = true
}

const RELEASE_NOTES_SANITIZE_CONFIG = Object.freeze({
  USE_PROFILES: { html: true },
  FORBID_TAGS: [
    'base',
    'form',
    'iframe',
    'input',
    'link',
    'meta',
    'object',
    'script',
    'style',
    'svg',
    'template',
  ],
  FORBID_ATTR: ['style'],
  ALLOW_DATA_ATTR: false,
  ALLOW_ARIA_ATTR: false,
  ALLOWED_URI_REGEXP: /^(?:https?:|data:image\/(?:gif|jpeg|png|webp);)/i,
})

export function renderReleaseNotes(source) {
  if (!source) return ''
  installLinkHook()
  return DOMPurify.sanitize(markdown.render(source), RELEASE_NOTES_SANITIZE_CONFIG)
}
