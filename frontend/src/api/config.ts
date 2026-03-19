import client from './client';

export interface BrandingConfig {
  chain_name: string;
  logo_url?: string;
  accent_color?: string;
  background_color_dark?: string;
  background_color_light?: string;
  success_color?: string;
  error_color?: string;
}

export async function getConfig(): Promise<BrandingConfig> {
  return client.get<BrandingConfig>('/config');
}
