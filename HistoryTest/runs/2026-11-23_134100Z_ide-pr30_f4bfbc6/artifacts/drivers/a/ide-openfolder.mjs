export default async (page) => {
  // Fire the click without waiting for the native dialog to finish.
  await page.getByRole('button', { name: 'Open folder' }).first().click({ noWaitAfter: true });
  return 'clicked Open folder';
};
