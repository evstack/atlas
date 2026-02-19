import { useMemo, useRef, useState } from 'react';

const DEFAULT_GATEWAYS = [
  'https://ipfs.io/ipfs/',
  'https://cloudflare-ipfs.com/ipfs/',
  'https://gateway.pinata.cloud/ipfs/',
];

function rewriteIpfs(url: string, gatewayBase: string): string {
  if (!url) return url;
  if (url.startsWith('ipfs://')) {
    const cid = url.replace('ipfs://', '').replace(/^ipfs\//, '');
    return gatewayBase + cid;
  }
  // Common pattern: https://ipfs.io/ipfs/<cid>
  const m = url.match(/https?:\/\/(?:[^/]+)\/ipfs\/(.+)$/);
  if (m) {
    return gatewayBase + m[1];
  }
  return url;
}

interface Props extends React.ImgHTMLAttributes<HTMLImageElement> {
  srcUrl?: string | null;
  gateways?: string[];
}

export default function ImageIpfs({ srcUrl, gateways = DEFAULT_GATEWAYS, alt = '', ...rest }: Props) {
  const [idx, setIdx] = useState(0);
  const triedRef = useRef(new Set<number>());
  const resolvedSrc = useMemo(() => {
    const gw = gateways[idx] || gateways[0];
    return srcUrl ? rewriteIpfs(srcUrl, gw) : '';
  }, [srcUrl, gateways, idx]);

  if (!srcUrl) return null;

  return (
    // eslint-disable-next-line jsx-a11y/alt-text
    <img
      {...rest}
      src={resolvedSrc}
      alt={alt}
      onError={() => {
        if (gateways.length === 0) return;
        triedRef.current.add(idx);
        // Find next gateway we haven't tried
        for (let i = 0; i < gateways.length; i++) {
          const next = (idx + 1 + i) % gateways.length;
          if (!triedRef.current.has(next)) { setIdx(next); return; }
        }
      }}
    />
  );
}

