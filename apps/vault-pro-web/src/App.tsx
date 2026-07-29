import { Navigate, Route, Routes } from "react-router-dom";
import { Shell } from "./components/Shell";
import { BatchDetailPage } from "./pages/BatchDetailPage";
import { BatchErrorsPage } from "./pages/BatchErrorsPage";
import { BatchesPage } from "./pages/BatchesPage";
import { RestorePage } from "./pages/RestorePage";
import { SubmitPage } from "./pages/SubmitPage";

export default function App() {
  return (
    <Routes>
      <Route element={<Shell />}>
        <Route index element={<Navigate to="/submit" replace />} />
        <Route path="submit" element={<SubmitPage />} />
        <Route path="restore" element={<RestorePage />} />
        <Route path="batches" element={<BatchesPage />} />
        <Route path="batches/:batchId" element={<BatchDetailPage />} />
        <Route path="batches/:batchId/errors" element={<BatchErrorsPage />} />
        <Route path="*" element={<Navigate to="/submit" replace />} />
      </Route>
    </Routes>
  );
}
