import type { Confirmation, ConfirmationService } from 'primeng/api';

/**
 * Confirmation dengan data tambahan untuk template dialog konfirmasi di shell:
 * `item` ditampilkan sebagai kartu (nama + kode), `note` sebagai catatan kecil.
 */
export type AppConfirmation = Confirmation & {
  item?: { name: string; code?: string };
  note?: string;
};

/** Konfirmasi hapus bergaya bahaya (ikon tong sampah merah, tombol Hapus merah). */
export function confirmDelete(
  confirm: ConfirmationService,
  options: {
    header: string;
    message: string;
    item?: AppConfirmation['item'];
    note?: string;
    acceptLabel?: string;
    accept: () => void;
    reject?: () => void;
  },
): void {
  const confirmation: AppConfirmation = {
    icon: 'pi pi-trash',
    acceptLabel: 'Hapus',
    rejectLabel: 'Batal',
    acceptIcon: 'pi pi-trash',
    acceptButtonProps: { severity: 'danger' },
    rejectButtonProps: { severity: 'secondary', outlined: true },
    ...options,
  };
  confirm.confirm(confirmation);
}
