import { useNavigate } from "react-router-dom";
import { ArrowLeft } from "lucide-react";

export function NotFound() {
  const navigate = useNavigate();
  return (
    <div className="page notfound">
      <span className="eyebrow mono">404 / NOT FOUND</span>
      <h1 className="display">页面不存在</h1>
      <p className="lede">请检查地址或返回市场行情。</p>
      <div className="inline-actions">
        <button className="btn btn-solid" onClick={() => navigate("/")}><ArrowLeft size={14} /> 回到市场</button>
      </div>
    </div>
  );
}
