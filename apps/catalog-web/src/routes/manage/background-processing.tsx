import { createFileRoute } from '@tanstack/react-router';
import { BackgroundProcessingPage } from '../../features/background-processing/BackgroundProcessingPage';

export const Route = createFileRoute('/manage/background-processing')({
  component: BackgroundProcessingPage,
});
