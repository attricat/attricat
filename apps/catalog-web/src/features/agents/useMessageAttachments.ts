import { useState } from 'react';

export const useMessageAttachments = () => {
  const [files, setFiles] = useState<File[]>([]);
  const [uploadedIds, setUploadedIds] = useState<string[]>([]);
  return {
    files,
    uploadedIds,
    setUploadedIds,
    replaceFiles: (next: File[]) => {
      setFiles(next);
      setUploadedIds([]);
    },
    removeFile: (file: File) => {
      setFiles((current) => current.filter((item) => item !== file));
      setUploadedIds([]);
    },
    reset: () => {
      setFiles([]);
      setUploadedIds([]);
    },
  };
};
